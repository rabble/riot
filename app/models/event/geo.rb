class Event < ActiveRecord::Base
  
  #check to see if an event has been geocoded
  def geocoded?
    latitude && longitude
  end
  
  def geocode!
    #self.attributes= GeoRiot::geocode_with_placemaker(self.attributes)
    self.attributes= GeoRiot::geocode_with_geonames(self.attributes)
  end
  
end