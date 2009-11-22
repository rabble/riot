
class GeoRiot < Object

  def self.geocode_with_plackemaker(options={})
    #TODO
    # extract this in do a delayed job
    places = RestClient.post('http://wherein.yahooapis.com/v1/document', 
          { :documentContent => options[:description], 
            :documentTitle => options[:title], 
            :apikey => gen_apikey, 
            :documentType => 'text/plain',
            :outputType => 'xml', 
            :autoDisambiguate => 'true'});
    h = Hpricot.XML(places)
    
    response = {}
    response[:location] = (h/:place/:name).inner_text
    response[:latitude] = (h/:place/:centroid/:latitude).inner_text
    response[:longitude] = (h/:place/:centroid/:longitude).inner_text
    
    return response
  end
  
  def self.geocode_with_geonames(options={})
    q = options['place']
    q ||= "#{options['title']} #{options['description']}"
    
    place_json = RestClient.post("http://ws.geonames.org/searchJSON", {'q'=> q, 'maxRows' =>   10})
    place_parsed = JSON.parse(place_json)
    return {} unless place_parsed['geonames'] 
    return {} if place_parsed['totalResultsCount'].to_i == 0
    
    first = place_parsed['geonames'].first
    location = Location.find_by_geonameid(first['geonameId']) | build_location(first)
    
  end
  
  def build_location(geonames_info)
    
    
    
    
  end
  
  def gen_apikey
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
  end
  
end