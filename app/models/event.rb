class Event < ActiveRecord::Base
  belongs_to :calendar
  belongs_to :location
  before_save :geocode!
  before_save :check_timezone
  
  concerned_with :parsing
  concerned_with :geo
  concerned_with :timezones
  
  #need to add catches for events which end within the span but dont' start, or which
  #neither start nor end within it, but are subsets of the span. ugh
  named_scope :on_day, lambda { |day|
    { :conditions => 
        [ "(starts_at >= ? and starts_at <= ?) or (starts_at <= ? and ends_at >= ?)", 
          day, day + 1.day, day, day ], 
      :order => 'starts_at' }
    }
  
  named_scope :in_month, lambda { |month|
    { :conditions => 
      [ "(starts_at >= ? and starts_at <= ?) or (starts_at <= ? and ends_at >= ?)", 
        month, month + 1.month, month, month ], 
      :order => 'starts_at' }
    }  

  named_scope :between, lambda { |starts,ends|
    { :conditions => 
      [ "(starts_at >= ? and starts_at <= ?) or (starts_at <= ? and ends_at >= ?)", 
        starts, ends, starts, starts ], 
      :order => 'starts_at' }
    }  
  
  def to_public_json
    self.to_json(:methods => [:start, :end, :starts_at_local, :ends_at_local])
  end
  

  
end
